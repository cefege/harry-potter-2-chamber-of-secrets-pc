//===============================================================================
// #63 Herman Wintringham
//===============================================================================

class  WCWintringham extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWintringhamTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wintringhamsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWintringhamBigTexture FILE=TEXTURES\menu\Folio\Cards\Wintringhambig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Herman Wintringham";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=63
    skin(0)=Texture'HProps.skins.WizardCardWintringhamTex0'
	textureBig=Texture'WizCardWintringhamBigTexture'
	strDescriptionId="WizCard_0069"
}
